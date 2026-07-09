<script lang="ts">
	class C {
		// readonly and declare are valid modifiers on a field
		readonly a = 1;
		declare b: number;
		// a plain method carries neither
		m() {}
	}

	// abstract / override on a METHOD are context-dependent early-errors (legal in an
	// abstract class / with a base class), not grammar violations, so they parse —
	// unlike readonly / declare, which are rejected on any method-like member
	abstract class A {
		abstract n(): void;
	}

	class B extends A {
		override n() {}
	}
</script>
