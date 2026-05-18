<script lang="ts">
interface A { a: // simple
X; b?: // optional
X; readonly c: // readonly
X; ['d']: // computed
X; e: // generic
Array<X>; }

type B = { f: // leading
X; // trailing
g: // c1
// c2
X; h: // leading
X; /* trailing */
};
</script>
