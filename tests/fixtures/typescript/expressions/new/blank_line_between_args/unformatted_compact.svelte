<script>
// Blank line between new expression args (no comments)
const a = new Foo(x,

y);

// Blank line between new expression args with leading comments
const b = new Foo(x,

// comment
y);

// Blank line with trailing line comment
const c = new Foo(x, // trailing

y);

// Mixed: some blank lines, some not
const d = new Foo(x,
y,

z);

// Leading comment before the FIRST arg, with a blank line after it: the blank-line
// layout must emit the `(`→first-argument gap itself or the comment is dropped
const e = new Foo(
// comment
x,

y);
</script>
