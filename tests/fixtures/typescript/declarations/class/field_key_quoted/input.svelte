<script lang="ts">
	// A quoted class field key keeps its quotes in every field form. Under `strict` the type
	// checker exempts a string-named field from the definite-assignment check it applies to an
	// identifier-named one, so the annotated field with no initializer is the form whose
	// quotes carry meaning.
	class A {
		'a': string;
		'b': number = 1;
		'c' = 2;
		'd';
		'fn' = () => 3;
		'e'?: number;
		'f'!: number;
	}

	// Modifiers and a decorator
	class B extends A {
		static 'g': string;
		static 'h' = 4;
		readonly 'i': string;
		private 'j': string;
		declare 'k': string;
		override 'c' = 5;
		@dec 'l': string;
	}

	// A class expression and an ambient class
	const C = class {
		'a': string;
	};
	declare class D {
		'a': string;
	}

	// A reserved word and an astral identifier are quoted keys like any other
	class E {
		'in': string;
		'𐊧': string;
	}

	// An unquoted sibling stays unquoted, a key that is no identifier stays quoted, an escape
	// stays as written, and a key holding a single quote keeps its double quotes
	class F {
		a = 1;
		'b' = 2;
		'c-d' = 3;
		'0' = 4;
		'\u0065' = 5;
		"f'g" = 6;
	}

	// Methods, accessors and the constructor are not fields: their keys unquote
	class G {
		'a': string;
		constructor() {}
		m() {}
		o(): void;
		o(b?: number): void {}
		async p() {}
		*q() {}
		get g() {
			return 1;
		}
		set s(v: number) {}
		static n() {}
	}
	abstract class H {
		abstract r(): void;
		t?(): void;
	}
</script>
