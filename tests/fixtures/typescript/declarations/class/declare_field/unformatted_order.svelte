<script lang="ts">
	// Basic declare
	class A {
		declare a: string;
	}

	// Declare with accessibility
	class B {
		public declare a: string;
		private declare b: string;
		protected declare c: string;
	}

	// Declare with static
	class C {
		static declare a: string;
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
