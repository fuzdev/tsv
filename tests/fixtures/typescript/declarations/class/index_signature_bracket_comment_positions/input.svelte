<script lang="ts">
	class A {
		[/* a */ k: string]: T;
	}
	class B {
		[k /* b */ : string]: T;
	}
	class C {
		[k: /* c */ string]: T;
	}
	class D {
		[k: string /* d */]: T;
	}
	// an own-line block comment keeps its own line and breaks the bracket
	class E {
		[
			/* e */
			k: string
		]: T;
	}
	// a glued run leads the key inline
	class F {
		[/* f */ /* g */ k: string]: T;
	}
	// an author blank line before the key is preserved
	class G {
		[
			/* h */

			k: string
		]: T;
	}
</script>
