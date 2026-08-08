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

	// The chain may continue past the private name
	class B {
		#a = { c: class {} };
		m() {
			new this.#a.c();
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
