<script lang="ts">
	// An auto-accessor is a field: its quoted key keeps its quotes
	class A {
		accessor "a": string;
		accessor "b" = 1;
		static accessor "c" = 2;
		@dec accessor "d": string;
	}

	// An abstract field and an abstract auto-accessor are fields too
	abstract class B {
		abstract "a": string;
		abstract readonly "b": number;
		abstract accessor "c": string;
	}
</script>
