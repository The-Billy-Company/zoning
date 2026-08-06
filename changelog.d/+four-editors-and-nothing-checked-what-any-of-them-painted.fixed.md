Four editors read `.zone` through four different runtimes - a TextMate grammar, a
Tree-sitter grammar with four queries, a Vim syntax file, and one language server behind
all of them - and the only thing CI asked of any of them was that it built. So the parts
that decide what you actually see on screen were the least checked code in the repo, and
they were wrong in five places nobody would have noticed by reading them.

`variance seal` painted `seal` as a statement keyword in Vim, in Zed, and in the server's
semantic tokens, because the word naming a law is spelled the same as the word opening a
statement. A `//` comment in Vim highlighted every keyword inside it, since the comment
rule was defined after the words it was supposed to swallow. Zone names like `floor.zig`
had no scope at all in VS Code, where Zed has painted them since the grammar shipped. The
server's semantic tokens matched keywords as substrings, so `package` lit up inside
`packages/**`, and the legend advertised five token types when the server only ever emits
two. VS Code's `increaseIndentPattern` only recognized one of the block forms the language
has, and nothing continued the line after `because`.

All five are fixed, and each of the four editors now has a suite that would have caught
its own: TextMate scopes tokenized through `vscode-textmate` exactly as the editor does,
Tree-sitter highlight annotations plus a pass requiring every query to still match what
Zed reads from it, `syntax.vim` assertions on the group at a line and column and what it
links to, and a protocol suite that answers every capability the server advertises - with
a gate that fails if `capabilities()` ever grows a sixth promise nothing keeps. The Vim
suites run under both `vim` and `nvim`, which disagree about enough to be worth it.
