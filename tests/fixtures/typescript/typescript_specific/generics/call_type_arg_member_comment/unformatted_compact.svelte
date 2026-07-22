<script lang="ts">
// Mapped type-arg, leading line comment
const a = fn<{
// comment
[K in keyof T]: V;
}>();

// Mapped type-arg, leading block comment
const b = fn<{
/* comment */
[K in keyof T]: V;
}>();

// Object type-literal type-arg, leading line comment
const c = fn<{
// comment
a: V;
}>();

// Object type-literal type-arg, leading block comment
const d = fn<{
/* comment */
a: V;
}>();

// Trailing member comment — outside the reparse region (control)
const e = fn<{
[K in keyof T]: V; // comment
}>();
</script>
