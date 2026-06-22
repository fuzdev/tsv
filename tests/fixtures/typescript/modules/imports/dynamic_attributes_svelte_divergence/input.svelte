<script lang="ts">
	// Basic import type with options
	type A = import('foo', { with: { type: 'json' } }).Bar;

	// With type arguments
	type B = import('foo', { with: { type: 'json' } }).Bar<T>;

	// Nested qualifier
	type C = import('foo', { with: { type: 'json' } }).Foo.Bar;

	// In union
	type D = import('foo', { with: { type: 'json' } }).Bar | null;
</script>
