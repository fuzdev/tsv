<script lang="ts">
	interface I {
		// comment inside the type parameter list stays inside
		m<T /* a */>(x: T): void;
		// comment after the optional ? (with type params) stays after ?
		o?/* b */ <T>(x: T): void;
	}
	type T2 = {
		n<U /* c */>(x: U): void;
		p?/* d */ <U>(x: U): void;
	};
</script>
