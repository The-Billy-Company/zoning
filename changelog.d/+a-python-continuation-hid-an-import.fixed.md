A Python import broken across lines with a backslash was read as two things, neither
of them right: the names after the break went missing from the graph, and the
backslash itself was reported as a module named `\`. So a file could import whatever
it liked as long as it wrapped the line, and the one law that catches an undeclared
dependency never fired.

Continuations are now joined where every other lexical question is answered - the
blanked copy of the source that already knows a comment from a string erases the
backslash and its newline to spaces, so a logical line reaches the dialect as one
line without the dialect knowing the rule exists. Line and column numbers still come
from the original text, so nothing a diagnostic points at moves.
