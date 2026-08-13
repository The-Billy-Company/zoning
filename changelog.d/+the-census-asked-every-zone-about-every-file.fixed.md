The census asked each zone how many files it held, and answering that question is
itself a walk over every zone - so the tally cost zones squared times files in glob
matches. On a drafted contract, where a zone per directory is the entire point, a
1400-zone package spent 22 seconds counting before it judged anything.

It now tallies in one pass over the files: 22s to under a second on the same package,
with the same numbers. A file two zones claim still counts for neither, because that
is a violation rather than a tenancy.
