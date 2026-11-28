<script lang="ts">
	// Every class-member modifier prints once, in the canonical order — the reordered
	// and repeated spellings normalize to it (see the variants).
	abstract class A extends Base {
		declare readonly a: number;
		public static readonly b = 1;
		protected abstract c: number;
		public abstract override readonly d: number;
		static accessor e = 1;
		static override readonly f = 1;
		public static async g() {}
		private static override async *h() {}
	}
</script>
