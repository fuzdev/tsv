<script lang="ts">
	interface A<in T> {
		fn(x: T): void;
	}

	interface B<out T> {
		fn(): T;
	}

	interface C<in out T> {
		fn(x: T): T;
	}

	type D<in T, out U> = (x: T) => U;
</script>
