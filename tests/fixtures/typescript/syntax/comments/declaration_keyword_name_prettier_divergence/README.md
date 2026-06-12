# declaration_keyword_name_prettier_divergence

Comments between modifier keywords are preserved in their original position.

- Input: `abstract /* b */ class B {}`
- Prettier: `abstract class /* b */ B {}` (moves to before name)
- Ours: `abstract /* b */ class B {}` (preserves between keywords)

Per comment placement policy, the user's chosen position is preserved.
Both positions are dual-stable in our formatter.
