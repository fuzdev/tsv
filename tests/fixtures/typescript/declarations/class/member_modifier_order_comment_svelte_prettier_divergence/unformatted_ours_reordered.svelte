<script lang="ts">
	// A reordered modifier run prints in canonical order, each comment after the keyword it
	// followed — so a line break stays after `static`, the one keyword it may follow
	abstract class A extends B {
		override public static // l
		x = 1;
		override public static /* m
		*/ y = 1;
		readonly static // l
		z = 1;
		readonly static /* m
		*/ w = 1;
		accessor static // l
		v = 1;
		accessor static /* m
		*/ u = 1;
		readonly /* a */ static /* b */ t = 1;
		@dec static // c
		public s = 1;
	}
</script>
