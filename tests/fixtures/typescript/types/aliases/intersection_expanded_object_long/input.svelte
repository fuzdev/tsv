<script lang="ts">
	// type parameters - 100 chars: the intersection stays on the `=` line
	type A<T extends U> = XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & {
		a: A;
	};

	// type parameters - 101 chars: the intersection breaks after `=`
	type B<T extends U> =
		XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & {
			a: A;
		};

	// no type parameters - 100 chars
	type C = XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & {
		a: A;
	};

	// no type parameters - 101 chars
	type D =
		XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & {
			a: A;
		};

	// several members - 100 chars to the first `&`: the first member stays on the `=` line
	type E<T extends U> = XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> &
		Y & {
			a: A;
		};

	// several members - 101 chars to the first `&`: the intersection breaks after `=`
	type F<T extends U> =
		XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> &
			Y & {
				a: A;
			};

	// a doc comment in the object - 100 chars: the intersection stays on the `=` line
	type J<T extends U> = XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & {
		/** c */
		a: A;
	};

	// a doc comment in the object - 101 chars: the intersection breaks after `=`
	type K<T extends U> =
		XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & {
			/** c */
			a: A;
		};

	// a constrained and defaulted parameter - 100 chars
	export type L<T extends U = U> = XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & {
		a: A;
	};

	// a constrained and defaulted parameter - 101 chars
	export type M<T extends U = U> =
		XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & {
			a: A;
		};

	// a flat object past the width: the intersection breaks after `=`
	type G<T extends U> =
		XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & { a: A };

	// two constrained parameters take `break-lhs`: the parameter list breaks instead
	type N<
		T extends U,
		V extends W
	> = XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & {
		a: A;
	};

	// a short intersection with a block comment stays on the `=` line
	type H<T extends U> = X<T> & /* c */ {
		a: A;
	};

	// a short intersection with a line comment stays on the `=` line
	type I<T extends U> = X<T> & // c
		Y;
</script>
