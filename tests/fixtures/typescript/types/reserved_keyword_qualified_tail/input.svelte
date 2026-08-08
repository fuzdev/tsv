<script lang="ts">
	// After `.` in an entity name, any IdentifierName is legal — reserved words
	// included (the HEAD position stays a `BindingIdentifier`)
	type A = ns.delete;
	type B = ns.false;
	type C = a.b.new.typeof;

	// A reserved tail may carry type arguments
	type D = ns.delete<number>;

	// A type QUERY takes the same tail names
	declare class E {
		delete(): void;
	}
	let a: typeof E.prototype.delete;

	// `this` is an IdentifierName in the tail position too
	class F {
		m() {
			let b: typeof this.this;
		}
	}
</script>
