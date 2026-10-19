<script lang="ts">
	// Identifier-valid string keys unquote, keywords included
	enum A {
		b = 'b',
		class = 1,
		constructor = 2,
		c = 3
	}

	// Keys that keep their quotes: an escape, a non-identifier, a numeric name
	enum B {
		'\x62' = 1,
		'd-2' = 2,
		'1' = 3
	}

	// A later initializer referencing an unquoted member
	enum C {
		a,
		b = a
	}

	// A comment beside a key stays where it was written
	enum I {
		b /* c */ = 1,
		/* d */ e = 2
	}

	// Every enum declaration form
	const enum D {
		a = 1
	}
	declare enum E {
		a = 1
	}
	export enum F {
		a = 1
	}
	export const enum G {
		a = 1
	}
	namespace N {
		export enum H {
			a = 1
		}
	}
</script>
