<script lang="ts">
	// A reordered modifier run prints in canonical order, each comment after the keyword it
	// followed — so a line break stays after `static`, the one keyword it may follow
	abstract class A extends B {
		public static override // l
		x = 1;
		public static override /* m
		 */ y = 1;
		static readonly // l
		z = 1;
		static readonly /* m
		 */ w = 1;
		static accessor // l
		v = 1;
		static accessor /* m
		 */ u = 1;
		static readonly /* b */ /* a */ t = 1;
		@dec // c
		public static s = 1;
	}
</script>
