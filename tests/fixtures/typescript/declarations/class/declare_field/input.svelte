<script lang="ts">
	// Basic declare
	class A {
		declare a: string;
	}

	// Declare with accessibility
	class B {
		declare public a: string;
		declare private b: string;
		declare protected c: string;
	}

	// Declare with static
	class C {
		declare static a: string;
	}

	// Declare with readonly
	class D {
		declare readonly a: string;
	}

	// Declare as property name (not modifier)
	class E {
		declare = 1;
	}
</script>
