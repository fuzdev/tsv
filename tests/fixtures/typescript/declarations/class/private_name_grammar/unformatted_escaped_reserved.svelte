<script lang="ts">
	// Reserved words are valid private names (`#` + IdentifierName)
	class A {
		#\u{64}efault = 0;
		#class = 1;
		#function() {}
		// literal keywords are IdentifierNames too
		#true = 2;
		#null = 3;
	}

	// Member access reads a reserved private name
	class B {
		#if = 0;
		fn() {
			return this.#if;
		}
	}

	// A brand check reads a reserved private name
	class C {
		#in;
		static check(x: object) {
			return #\u{69}n in x;
		}
	}

	// Static, accessor, and generator members take reserved private names
	class D {
		static #delete = 0;
		get #typeof() {
			return D.#delete;
		}
		set #typeof(v: number) {
			D.#delete = v;
		}
		*#for() {}
	}
</script>
