<script lang="ts">
	// a frozen member keeps its separator as authored, or its lack of one
	type A = {
		// prettier-ignore
		a:  1
		;
		// prettier-ignore
		b:  1 /* c */
		;
		// prettier-ignore
		c:  1, // c
		// prettier-ignore
		d:  1

		;
		// prettier-ignore
		e:  1
		,
		// prettier-ignore
		f:  1,
		// prettier-ignore
		g:  1
		h: 1;
		// prettier-ignore
		i:  1  ;
		j: 1;
	};
	// the last member, with a line comment, with nothing, with an own-line separator
	type B1 = {
		// prettier-ignore
		a:  1 // c
	};
	type B2 = {
		// prettier-ignore
		a:  1
	};
	type B3 = {
		// prettier-ignore
		a:  1
		;
	};
	// the interface twins
	interface I {
		// prettier-ignore
		a:  1
		;
		// prettier-ignore
		b:  1 /* c */
		;
		// prettier-ignore
		c:  1, // c
		// prettier-ignore
		d:  1

		;
		// prettier-ignore
		e:  1
		,
		// prettier-ignore
		f:  1,
		// prettier-ignore
		g:  1
		h: 1;
		// prettier-ignore
		i:  1  ;
		j: 1;
	}
	interface J1 {
		// prettier-ignore
		a:  1 // c
	}
	interface J2 {
		// prettier-ignore
		a:  1
	}
	interface J3 {
		// prettier-ignore
		a:  1
		;
	}
</script>
