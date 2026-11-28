<script lang="ts">
	// A reordered modifier run prints in canonical order, each comment after the keyword it
	// followed — so a line break stays after `static`, the one keyword it may follow
	abstract class A extends B {
		public static // l
		override x = 1;
		public static /* m
		 */ override y = 1;
		static // l
		readonly z = 1;
		static /* m
		 */ readonly w = 1;
		static // l
		accessor v = 1;
		static /* m
		 */ accessor u = 1;
		static /* b */ readonly /* a */ t = 1;
		@dec public static // c
		s = 1;
	}
</script>
