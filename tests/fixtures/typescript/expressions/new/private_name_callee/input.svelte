<script lang="ts">
	// A private name is a valid `new` callee member
	class A {
		#a = class {};
		static #b = class {};
		m() {
			new this.#a();
		}
		static n() {
			new A.#b();
		}
	}

	// The chain may continue past the private name, through either subscript form
	class B {
		#a = { c: class {} };
		#b = [class {}];
		m() {
			new this.#a.c();
			new this.#b[0]();
		}
	}

	// Non-`new` private access is unaffected
	class C {
		#a = function () {};
		m() {
			this.#a();
			this.#a?.();
		}
	}
</script>
