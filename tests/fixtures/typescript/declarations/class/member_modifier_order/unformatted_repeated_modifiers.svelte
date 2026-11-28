<script lang="ts">
	// Every class-member modifier prints once, in the canonical order — the reordered
	// and repeated spellings normalize to it (see the variants).
	abstract class A extends Base {
		declare declare readonly readonly a: number;
		public static readonly readonly b = 1;
		protected abstract abstract c: number;
		public abstract override override readonly d: number;
		static accessor accessor e = 1;
		override static override readonly f = 1;
		public static async async g() {}
		private static override async async *h() {}
	}
</script>
