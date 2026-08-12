<script lang="ts">
// Anonymous class expression - block comment between keyword and type params
const a = class/* c1 */<T> {};

// With heritage
const b = class/* c2 */<T> extends A<T> {};

// Default-exported anonymous class
export default class/* c3 */<T> {}
</script>
