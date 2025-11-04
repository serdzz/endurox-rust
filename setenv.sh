#!/bin/sh

# Enduro/X environment setup

# Application home
export NDRX_APPHOME="${NDRX_APPHOME:-/app}"

# Enduro/X installation path (adjust to your installation)
export NDRX_HOME=${NDRX_HOME:-/opt/endurox}

# Node ID (unique identifier for this instance)
#export NDRX_NODEID=1

# Queue prefix (for message queues)
#export NDRX_QPREFIX=/n

# Queue path (directory for persistent queues)
#export NDRX_QPATH=${NDRX_APPHOME}/tmp

# Resource identifier (random key for resource management)
#export NDRX_RNDK=ndrxrndk

# Maximum message size (in bytes)
#export NDRX_MSGSIZEMAX=65536

# Maximum number of messages in queue
#export NDRX_MSGMAX=100

# Maximum number of servers
#export NDRX_SRVMAX=20

# Maximum number of services
#export NDRX_SVCMAX=50

# IPC key (0 means automatic generation)
#xport NDRX_IPCKEY=0

# PID file for ndrxd backend process
#export NDRX_CONFIG=${NDRX_APPHOME}/ndrxconfig.xml

# Debug configuration
export NDRX_DEBUG_CONF=${NDRX_APPHOME}/debug.conf

# Common configuration
export NDRX_CCONFIG=${NDRX_APPHOME}/conf/ndrxconf.ini

# UBF field table directory
export FLDTBLDIR=${NDRX_APPHOME}/ubftab

# View files directory
export VIEWDIR=${NDRX_APPHOME}/views

# Additional library path
export LD_LIBRARY_PATH=${NDRX_HOME}/lib:${NDRX_APPHOME}/lib:${LD_LIBRARY_PATH}
export DYLD_LIBRARY_PATH=${NDRX_HOME}/lib:${NDRX_APPHOME}/lib:${DYLD_LIBRARY_PATH}

# Binary path
export PATH=${NDRX_HOME}/bin:${NDRX_APPHOME}/bin:${PATH}

# Create necessary directories
mkdir -p ${NDRX_APPHOME}/log
mkdir -p ${NDRX_APPHOME}/tmp
mkdir -p ${NDRX_APPHOME}/conf
mkdir -p ${NDRX_APPHOME}/ubftab
mkdir -p ${NDRX_APPHOME}/views
mkdir -p ${NDRX_APPHOME}/bin
mkdir -p ${NDRX_APPHOME}/lib

echo "Enduro/X environment configured"
echo "NDRX_APPHOME: ${NDRX_APPHOME}"
echo "NDRX_CONFIG: ${NDRX_CONFIG}"
