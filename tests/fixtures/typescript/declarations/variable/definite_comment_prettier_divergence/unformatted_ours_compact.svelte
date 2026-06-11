<script lang="ts">
// Comment between name and ! - prettier relocates to after !
let a/* c1 */!: number;

// With initializer
let b/* c2 */!: number = 1;

// Without type annotation
let c/* c3 */! =1;
let d/* c4 */!;
</script>
