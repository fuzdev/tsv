// `let` heads a declaration only when a binding follows it. In every other for-head
// spelling it stays the `IdentifierReference` the production admits, and the head is
// an ordinary expression.
for (let; ;) {}

for (let = 3; ;) {}

for (let instanceof o; ;) {}

// a for-in left keeps the paren the printer draws across both head forms
for ((let) in o) {
}

for ((let).x in o) {
}

// an array literal heads an expression too - `let` is an element, not a binding
for ([let][1] in o) {
}

// a binding does follow, so these stay declarations
for (let x of y) {
}

for (let [a] in o) {
}

// `let of` binds `of`, and the second `of` is the for-of keyword
for (let of of x) {
}
