// a single-statement position takes a `Statement`, and a `LexicalDeclaration` is not
// one - so `let` there is an `IdentifierReference` and the statement closes at it
L: let
a = 1;

if (a) let
else b;

while (a) let
c = 1;

for (;;) let
d = 1;

if (a) let
{
}
