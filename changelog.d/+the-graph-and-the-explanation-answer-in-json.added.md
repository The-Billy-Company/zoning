`zone map --json` emits the whole import graph, and `zone explain --json` answers both
of its questions in the machine form: one file's standing, or whether one import is
allowed. Previously only `verify` and `status` had a machine form, and `explain
--json` accepted the flag and printed prose anyway - which is worse than refusing it,
since a script asking for JSON and getting a report has no way to tell.

Every edge and every departure in the graph carries the zone each end sits in,
resolved during the pass that already did the glob matching, so a consumer never
re-derives where a file lives. A verb with no machine form now refuses `--json` and
says which verbs have one.
