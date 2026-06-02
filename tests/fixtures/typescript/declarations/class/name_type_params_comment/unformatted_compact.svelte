<script lang="ts">
// Block comment between class name and type params
class A/* c1 */<T> {}

// Class expression - block comment between name and type params
const a = class B/* c2 */<T> {};

// Block comment between type params and body
class C<T>/* c3 */{}

// Class expression - block comment between type params and body
const b = class D<T>/* c4 */{};
</script>
