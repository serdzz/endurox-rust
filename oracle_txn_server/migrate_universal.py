#!/usr/bin/env python3
"""
Universal Migration Tool for PostgreSQL and Oracle
Usage:
    python migrate_universal.py run      - Apply all pending migrations
    python migrate_universal.py rollback - Rollback the last migration
    python migrate_universal.py status   - Show migration status
    python migrate_universal.py reset    - Rollback all migrations
"""

import os
import sys
from pathlib import Path
from urllib.parse import urlparse

def parse_database_url(url):
    """Parse DATABASE_URL and determine database type"""
    parsed = urlparse(url)
    
    if parsed.scheme in ('postgres', 'postgresql'):
        return 'postgres', {
            'url': url,
            'host': parsed.hostname,
            'port': parsed.port or 5432,
            'user': parsed.username,
            'password': parsed.password,
            'database': parsed.path.lstrip('/')
        }
    elif parsed.scheme == 'oracle':
        return 'oracle', {
            'url': url,
            'host': parsed.hostname,
            'port': parsed.port or 1521,
            'user': parsed.username,
            'password': parsed.password,
            'service': parsed.path.lstrip('/')
        }
    else:
        raise ValueError(f"Unsupported database URL scheme: {parsed.scheme}")

# Get DATABASE_URL
DATABASE_URL = os.environ.get('DATABASE_URL')
if not DATABASE_URL:
    print("ERROR: DATABASE_URL environment variable not set")
    print("Examples:")
    print("  export DATABASE_URL=postgres://user:pass@host:port/database")
    print("  export DATABASE_URL=oracle://user:pass@host:port/service")
    sys.exit(1)

# Determine database type
try:
    DB_TYPE, DB_CONFIG = parse_database_url(DATABASE_URL)
    print(f"Detected database type: {DB_TYPE.upper()}")
    if DB_TYPE == 'postgres':
        print(f"Connection: {DB_CONFIG['user']}@{DB_CONFIG['host']}:{DB_CONFIG['port']}/{DB_CONFIG['database']}")
    else:
        print(f"Connection: {DB_CONFIG['user']}@{DB_CONFIG['host']}:{DB_CONFIG['port']}/{DB_CONFIG['service']}")
except Exception as e:
    print(f"ERROR: Failed to parse DATABASE_URL: {e}")
    sys.exit(1)

MIGRATIONS_BASE = Path(__file__).parent / "migrations"
MIGRATIONS_DIR = MIGRATIONS_BASE / DB_TYPE

if not MIGRATIONS_DIR.exists():
    print(f"ERROR: Migrations directory not found: {MIGRATIONS_DIR}")
    sys.exit(1)

class PostgresMigrationManager:
    def __init__(self, config):
        self.config = config
        self.connection = None
        self.cursor = None
    
    def __enter__(self):
        import psycopg2
        self.connection = psycopg2.connect(
            host=self.config['host'],
            port=self.config['port'],
            user=self.config['user'],
            password=self.config['password'],
            database=self.config['database']
        )
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
        self.cursor.execute("""
            CREATE TABLE IF NOT EXISTS __diesel_schema_migrations (
                version VARCHAR(50) PRIMARY KEY,
                run_on TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )""")
        self.connection.commit()
    
    def _get_applied_migrations(self):
        """Get list of applied migration versions"""
        self.cursor.execute('SELECT version FROM __diesel_schema_migrations ORDER BY version')
        return [row[0] for row in self.cursor.fetchall()]
    
    def _execute_sql(self, sql):
        """Execute SQL statement(s)"""
        self.cursor.execute(sql)
    
    def commit(self):
        self.connection.commit()
    
    def rollback(self):
        self.connection.rollback()

class OracleMigrationManager:
    def __init__(self, config):
        self.config = config
        self.connection = None
        self.cursor = None
    
    def __enter__(self):
        import oracledb
        dsn = f"{self.config['host']}:{self.config['port']}/{self.config['service']}"
        self.connection = oracledb.connect(
            user=self.config['user'],
            password=self.config['password'],
            dsn=dsn
        )
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
        except Exception as e:
            if 'ORA-00955' not in str(e):  # Table already exists
                raise
    
    def _get_applied_migrations(self):
        """Get list of applied migration versions"""
        self.cursor.execute('SELECT version FROM "__diesel_schema_migrations" ORDER BY version')
        return [row[0] for row in self.cursor.fetchall()]
    
    def _execute_sql(self, sql):
        """Execute SQL statement(s) - handles PL/SQL blocks"""
        statements = []
        current_statement = []
        in_plsql_block = False
        
        for line in sql.split('\n'):
            stripped = line.strip().upper()
            
            if stripped.startswith('--') or (not stripped and not in_plsql_block):
                continue
            
            if not in_plsql_block and ('CREATE OR REPLACE TRIGGER' in stripped or 
                                        stripped.startswith('DECLARE') or 
                                        stripped.startswith('BEGIN')):
                in_plsql_block = True
            
            current_statement.append(line)
            
            if stripped.endswith(';'):
                if in_plsql_block and 'END;' in stripped:
                    in_plsql_block = False
                    statements.append('\n'.join(current_statement).strip())
                    current_statement = []
                elif not in_plsql_block:
                    statements.append('\n'.join(current_statement).strip())
                    current_statement = []
        
        if current_statement:
            statements.append('\n'.join(current_statement).strip())
        
        for statement in statements:
            if statement:
                statement = statement.rstrip(';') if not statement.upper().endswith('END;') else statement
                self.cursor.execute(statement)
    
    def commit(self):
        self.connection.commit()
    
    def rollback(self):
        self.connection.rollback()

def get_manager():
    """Get appropriate migration manager based on database type"""
    if DB_TYPE == 'postgres':
        return PostgresMigrationManager(DB_CONFIG)
    else:
        return OracleMigrationManager(DB_CONFIG)

def get_available_migrations():
    """Get list of available migration directories"""
    if not MIGRATIONS_DIR.exists():
        return []
    
    migrations = []
    for item in sorted(MIGRATIONS_DIR.iterdir()):
        if item.is_dir() and not item.name.startswith('.'):
            migrations.append(item.name)
    return migrations

def status():
    """Show migration status"""
    with get_manager() as manager:
        available = get_available_migrations()
        applied = manager._get_applied_migrations()
        
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

def run():
    """Apply all pending migrations"""
    with get_manager() as manager:
        available = get_available_migrations()
        applied = manager._get_applied_migrations()
        pending = [m for m in available if m not in applied]
        
        if not pending:
            print("\n✓ All migrations are already applied")
            return
        
        print(f"\nApplying {len(pending)} migration(s)...")
        
        for migration in pending:
            migration_dir = MIGRATIONS_DIR / migration
            up_file = migration_dir / "up.sql"
            
            if not up_file.exists():
                print(f"✗ Migration file not found: {up_file}")
                continue
            
            print(f"⬆ Running migration: {migration}")
            
            try:
                with open(up_file, 'r') as f:
                    sql = f.read()
                
                manager._execute_sql(sql)
                manager.cursor.execute(
                    'INSERT INTO __diesel_schema_migrations (version) VALUES (%s)' if DB_TYPE == 'postgres'
                    else 'INSERT INTO "__diesel_schema_migrations" (version) VALUES (:1)',
                    (migration,)
                )
                manager.commit()
                print(f"✓ Applied: {migration}")
            
            except Exception as e:
                print(f"✗ Failed to apply migration {migration}: {e}")
                manager.rollback()
                sys.exit(1)
        
        print(f"\n✓ Successfully applied {len(pending)} migration(s)")

def rollback(count=1):
    """Rollback last N migrations"""
    with get_manager() as manager:
        applied = manager._get_applied_migrations()
        
        if not applied:
            print("\n✓ No migrations to rollback")
            return
        
        to_rollback = applied[-count:]
        print(f"\nRolling back {len(to_rollback)} migration(s)...")
        
        for migration in reversed(to_rollback):
            migration_dir = MIGRATIONS_DIR / migration
            down_file = migration_dir / "down.sql"
            
            if not down_file.exists():
                print(f"✗ Rollback file not found: {down_file}")
                continue
            
            print(f"⬇ Rolling back: {migration}")
            
            try:
                with open(down_file, 'r') as f:
                    sql = f.read()
                
                manager._execute_sql(sql)
                manager.cursor.execute(
                    'DELETE FROM __diesel_schema_migrations WHERE version = %s' if DB_TYPE == 'postgres'
                    else 'DELETE FROM "__diesel_schema_migrations" WHERE version = :1',
                    (migration,)
                )
                manager.commit()
                print(f"✓ Rolled back: {migration}")
            
            except Exception as e:
                print(f"✗ Failed to rollback migration {migration}: {e}")
                manager.rollback()
                sys.exit(1)
        
        print(f"\n✓ Successfully rolled back {len(to_rollback)} migration(s)")

def reset():
    """Rollback all migrations"""
    with get_manager() as manager:
        applied = manager._get_applied_migrations()
        if applied:
            rollback(len(applied))
        else:
            print("\n✓ No migrations to rollback")

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)
    
    command = sys.argv[1]
    
    if command == "status":
        status()
    elif command == "run":
        run()
    elif command == "rollback":
        count = int(sys.argv[2]) if len(sys.argv) > 2 else 1
        rollback(count)
    elif command == "reset":
        reset()
    else:
        print(f"Unknown command: {command}")
        print(__doc__)
        sys.exit(1)
