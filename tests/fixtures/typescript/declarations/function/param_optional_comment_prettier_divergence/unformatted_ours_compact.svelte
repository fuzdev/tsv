<script lang="ts">
// Comment between param name and ? - prettier relocates to after ?
function fn1(a/* c1 */?: number) {}

// Multiple params
function fn2(b/* c2 */?: number, c?: string) {}
</script>
