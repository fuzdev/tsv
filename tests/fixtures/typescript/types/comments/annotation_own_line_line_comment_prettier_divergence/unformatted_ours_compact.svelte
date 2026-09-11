<script lang="ts">
// An own-line `//` in an annotation's `:`→type gap keeps the line the author gave it; the
// type hangs one level in below it.
let a:
// c1
B;

// ...at a parameter, a class property and a property signature.
function fn1(
b:
// c2
B
) {}

class C {
c:
// c3
B;
}

interface I {
d:
// c4
B;
}

// ...at every return type.
function fn2():
// c5
B {}

const fn3 = ():
// c6
B => e;

interface J {
fn4():
// c7
B;
}

// ...and at an index signature's value.
interface K {
[k: string]:
// c8
B;
}

// A union, an intersection and a hugging union hang as a simple type does.
let f:
// c9
A | B;
let g:
// c10
A & B;
let h:
// c11
{ a: 1 } | null;

// A run keeps one comment per line, in order.
let i:
// c12
// c13
B;

// A block ahead of the line comment keeps its own line too.
let j:
/* c14 */
// c15
B;

// A block on the `:` line stays there, and the author blank after it survives.
let k:/* c16 */

// c17
B;

// Control: a comment the author put ON the `:` line trails it, as before.
let l:// c18
B;
</script>
