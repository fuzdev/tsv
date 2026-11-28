<script lang="ts">
	// Every class-member modifier prints once, in the canonical order — the reordered
	// and repeated spellings normalize to it (see the variants).
	abstract class A extends Base {
		readonly declare a: number;
		readonly static public b = 1;
		abstract protected c: number;
		readonly override abstract public d: number;
		accessor static e = 1;
		readonly override static f = 1;
		async static public g() {}
		async override static private *h() {}
	}
</script>
