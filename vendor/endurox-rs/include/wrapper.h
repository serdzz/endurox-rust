#include <ndrx_config.h>
#include <ndebug.h>
#include <xatmi.h>
#include <oatmi.h>
#include <oatmisrv.h>
#include <oatmisrv_integra.h>
#include <ubf.h>
#include <oubf.h>
#include <nerror.h>
#include <nstdutil.h>

/* libatmisrvinteg exports these worker lifecycle hooks but public headers do
 * not declare the variables. Language integrations must install the default
 * hooks when enabling dispatch-thread mode. */
extern int (*ndrx_G_tpsvrthrinit)(int argc, char **argv);
extern void (*ndrx_G_tpsvrthrdone)(void);

/* Enduro/X releases up to 8.0.x do not define TPQKEEPORIG yet. Provide a
 * fallback so the bindings build against them; newer headers win. */
#ifndef TPQKEEPORIG
#define TPQKEEPORIG 0x200000
#endif
