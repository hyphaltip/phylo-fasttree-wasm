/*
 * Reset the file-scope state that FastTree's main() leaves behind, so that
 * fasttree_build() gives identical results when called repeatedly in one
 * wasm instance. The algorithm itself is unchanged.
 */
#include <stdbool.h>

extern int verbose;
extern int showProgress;
extern int slow;
extern int fastest;
extern bool useTopHits2nd;
extern int bionj;
extern double tophitsMult;
extern double tophitsClose;
extern double tophitsRefresh;
extern int nCodes;
extern bool useMatrix;
extern bool logdist;
extern double pseudoWeight;
extern double constraintWeight;
extern bool fastNNI;
extern bool gammaLogLk;
extern int mlAccuracy;
extern bool exactML;
extern unsigned char *codesString;
extern unsigned char *codesStringAA;
extern long nSPR;
extern long nSuboptimalSplits;
extern long nSuboptimalConstrained;
extern long nConstraintViolations;

void ran_start(long seed); /* defined in FastTree.c */

void ft_reset_fasttree_globals(void)
{
    verbose = 1;
    showProgress = 1;
    slow = 0;
    fastest = 0;
    useTopHits2nd = false;
    bionj = 0;
    tophitsMult = 1.0;
    tophitsClose = -1.0;
    tophitsRefresh = 0.8;
    useMatrix = true;
    logdist = true;
    pseudoWeight = 0.0;
    constraintWeight = 100.0;
    fastNNI = true;
    gammaLogLk = false;
    mlAccuracy = 1;
    exactML = true;
    nCodes = 20;
    codesString = codesStringAA;

    nSPR = 0;
    nSuboptimalSplits = 0;
    nSuboptimalConstrained = 0;
    nConstraintViolations = 0;

    /* FastTree auto-seeds lazily; without this, call #2 continues call #1's
       random stream and can produce a different tree. */
    ran_start(314159L);
}
