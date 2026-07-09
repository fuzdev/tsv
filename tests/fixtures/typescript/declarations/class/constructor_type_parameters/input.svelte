<script lang="ts">
	class C {
		// a plain constructor takes no type parameters
		constructor() {}
		// a generic method (not the constructor) may have type parameters
		m<T>(): void {}
		// a static method named `constructor` is an ordinary method, so type params are fine
		static constructor<T>(): void {}
	}
</script>
