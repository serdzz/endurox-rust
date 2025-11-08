#!/usr/bin/env python3
"""
Oracle Migration Tool
Usage:
    python migrate.py run      - Apply all pending migrations
    python migrate.py rollback - Rollback the last migration
    python migrate.py status   - Show migration status
    python migrate.py reset    - Rollback all migrations
"""

import oracledb
import os
import sys
from pathlib import Path
from datetime import datetime
from urllib.parse import urlparse

def parse_database_url(url):
    """
    Parse DATABASE_URL in format: oracle://user:password@host:port/service
    Returns dict with connection parameters for oracledb
    """
    parsed = urlparse(url)
    return {
        "user": parsed.username,
        "password": parsed.password,
        "dsn": f"{parsed.hostname}:{parsed.port or 1521}{parsed.path}"
    }

# Connection parameters
# Priority: DATABASE_URL > individual env vars > defaults
if os.environ.get("DATABASE_URL"):
    DB_CONFIG = parse_database_url(os.environ["DATABASE_URL"])
    print(f"Using DATABASE_URL: {DB_CONFIG['dsn']}")
else:
    # Fallback to individual environment variables
    DB_HOST = os.environ.get("ORACLE_HOST", "oracledb")
    DB_PORT = os.environ.get("ORACLE_PORT", "1521")
    DB_USER = os.environ.get("ORACLE_USER", "ctp")
    DB_PASSWORD = os.environ.get("ORACLE_PASSWORD", "ctp")
    DB_SERVICE = os.environ.get("ORACLE_SERVICE", "XE")
    
    DB_CONFIG = {
        "user": DB_USER,
        "password": DB_PASSWORD,
        "dsn": f"{DB_HOST}:{DB_PORT}/{DB_SERVICE}"
    }
    print(f"Using connection: {DB_CONFIG['user']}@{DB_CONFIG['dsn']}")

MIGRATIONS_DIR = Path(__file__).parent / "migrations"

class MigrationManager:
    def __init__(self, config):
        self.config = config
        self.connection = None
        self.cursor = None
    
    def __enter__(self):
        self.connection = oracledb.connect(**self.config)
        self.cursor = self.connection.cursor()
        self._ensure_migrations_table()
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        if self.cursor:
            self.cursor.close()
        if self.connection:
            self.connection.close()
    
    def _ensure_migrations_table(self):
        """Create migrations tracking table if it doesn't exist"""
        try:
            self.cursor.execute("""
                CREATE TABLE "__diesel_schema_migrations" (
                    version VARCHAR2(50) PRIMARY KEY,
                    run_on TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )""")
            self.connection.commit()
            print("Created migrations tracking table")
        except oracledb.DatabaseError as e:
            error, = e.args
            if error.code != 955:  # Table already exists
                raise
    
    def _get_applied_migrations(self):
        """Get list of applied migration versions"""
        self.cursor.execute('SELECT version FROM "__diesel_schema_migrations" ORDER BY version')
        return [row[0] for row in self.cursor.fetchall()]
    
    def _get_available_migrations(self):
        """Get list of available migration directories"""
        if not MIGRATIONS_DIR.exists():
            return []
        
        migrations = []
        for item in sorted(MIGRATIONS_DIR.iterdir()):
            if item.is_dir() and not item.name.startswith('.'):
                migrations.append(item.name)
        return migrations
    
    def _read_sql_file(self, filepath):
        """Read SQL file and return its content"""
        with open(filepath, 'r') as f:
            return f.read().strip()
    
    def _execute_sql(self, sql):
        """Execute SQL statement(s) - handles PL/SQL blocks"""
        # For PL/SQL blocks (CREATE TRIGGER, BEGIN/END), we need smart parsing
        # Simple approach: split by semicolon, but treat BEGIN...END as one block
        statements = []
        current_statement = []
        in_plsql_block = False
        
        for line in sql.split('\n'):
            stripped = line.strip()
            
            # Skip comment-only lines and empty lines
            if not stripped or stripped.startswith('--'):
                continue
            
            # Check if we're entering a PL/SQL block
            if 'BEGIN' in stripped.upper() and not in_plsql_block:
                in_plsql_block = True
            
            current_statement.append(line)
            
            # Check if statement ends
            if stripped.endswith(';'):
                if in_plsql_block:
                    # Check if this is END; which closes the block
                    if 'END' in stripped.upper():
                        in_plsql_block = False
                        statement_text = '\n'.join(current_statement).strip()
                        if statement_text:
                            statements.append(statement_text)
                        current_statement = []
                else:
                    # Regular statement
                    statement_text = '\n'.join(current_statement).strip()
                    if statement_text:
                        statements.append(statement_text)
                    current_statement = []
        
        # Add any remaining statement
        if current_statement:
            statement_text = '\n'.join(current_statement).strip()
            if statement_text:
                statements.append(statement_text)
        
        # Execute each statement
        for i, statement in enumerate(statements, 1):
            if statement:
                # Remove trailing semicolon (Oracle python driver doesn't want it)
                statement = statement.rstrip().rstrip(';')
                self.cursor.execute(statement)
    
    def status(self):
        """Show migration status"""
        available = self._get_available_migrations()
        applied = self._get_applied_migrations()
        
        print("\nMigration Status:")
        print("-" * 60)
        
        if not available:
            print("No migrations found")
            return
        
        for migration in available:
            status = "✓ Applied" if migration in applied else "✗ Pending"
            print(f"{status:12} {migration}")
        
        print("-" * 60)
        print(f"Total: {len(available)} migrations, {len(applied)} applied, {len(available) - len(applied)} pending")
    
    def run(self):
        """Apply all pending migrations"""
        available = self._get_available_migrations()
        applied = self._get_applied_migrations()
        pending = [m for m in available if m not in applied]
        
        if not pending:
            print("No pending migrations")
            return
        
        print(f"\nApplying {len(pending)} migration(s)...")
        
        for migration in pending:
            up_file = MIGRATIONS_DIR / migration / "up.sql"
            
            if not up_file.exists():
                print(f"⚠ Skipping {migration}: up.sql not found")
                continue
            
            try:
                print(f"⬆ Running migration: {migration}")
                sql = self._read_sql_file(up_file)
                self._execute_sql(sql)
                
                # Record migration
                self.cursor.execute(
                    'INSERT INTO "__diesel_schema_migrations" (version) VALUES (:1)',
                    [migration]
                )
                self.connection.commit()
                print(f"✓ Applied: {migration}")
                
            except Exception as e:
                self.connection.rollback()
                print(f"✗ Error applying {migration}: {e}")
                raise
        
        print(f"\n✓ Successfully applied {len(pending)} migration(s)")
    
    def rollback(self, steps=1):
        """Rollback the last N migration(s)"""
        applied = self._get_applied_migrations()
        
        if not applied:
            print("No migrations to rollback")
            return
        
        to_rollback = applied[-steps:] if steps > 0 else []
        
        if not to_rollback:
            print("No migrations to rollback")
            return
        
        print(f"\nRolling back {len(to_rollback)} migration(s)...")
        
        for migration in reversed(to_rollback):
            down_file = MIGRATIONS_DIR / migration / "down.sql"
            
            if not down_file.exists():
                print(f"⚠ Skipping {migration}: down.sql not found")
                continue
            
            try:
                print(f"⬇ Rolling back: {migration}")
                sql = self._read_sql_file(down_file)
                self._execute_sql(sql)
                
                # Remove migration record
                self.cursor.execute(
                    'DELETE FROM "__diesel_schema_migrations" WHERE version = :1',
                    [migration]
                )
                self.connection.commit()
                print(f"✓ Rolled back: {migration}")
                
            except Exception as e:
                self.connection.rollback()
                print(f"✗ Error rolling back {migration}: {e}")
                raise
        
        print(f"\n✓ Successfully rolled back {len(to_rollback)} migration(s)")
    
    def reset(self):
        """Rollback all migrations"""
        applied = self._get_applied_migrations()
        if applied:
            self.rollback(steps=len(applied))
        else:
            print("No migrations to reset")


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)
    
    command = sys.argv[1].lower()
    
    try:
        with MigrationManager(DB_CONFIG) as manager:
            if command == "run":
                manager.run()
            elif command == "rollback":
                steps = int(sys.argv[2]) if len(sys.argv) > 2 else 1
                manager.rollback(steps)
            elif command == "status":
                manager.status()
            elif command == "reset":
                manager.reset()
            else:
                print(f"Unknown command: {command}")
                print(__doc__)
                sys.exit(1)
    
    except oracledb.DatabaseError as e:
        error, = e.args
        print(f"\n✗ Database Error: {error.message}")
        sys.exit(1)
    except Exception as e:
        print(f"\n✗ Error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
