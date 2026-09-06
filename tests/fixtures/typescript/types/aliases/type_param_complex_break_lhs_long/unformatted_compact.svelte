<script lang="ts">
	// Prettier's `isComplexTypeAliasParams` - two or more type parameters, at least one
	// constrained or defaulted - takes `break-lhs`: the `<...>` list breaks, the `=` stays on
	// the closing `>` line, and the value rides beside it. The list breaks only once the head
	// plus the value's first line - up to its own first break point - overflows. Asked AHEAD
	// of the value-shaped layouts (the intersection's fluid marker, a reference's hug) and
	// only BEHIND the break-after-operator ones (a non-hugging union, a conditional with
	// generic halves).

	// 100 - the head reaches the first member's `<` exactly at the width: the list stays flat
	type Aaaa<T,Uuuuuuuuuuuu extends Bbbbbbbbbbbbbbbbbb=Bbbbbbbbbbbbbbbbb>=Wwwwwwwwwwwwwwwwwwwww<T,U> &Cccc<T>;

	// 101 - one column more and the list breaks; the value rides the `>` line
	type Aaaaa<T,Uuuuuuuuuuuu extends Bbbbbbbbbbbbbbbbbb=Bbbbbbbbbbbbbbbbb>=Wwwwwwwwwwwwwwwwwwwww<T,U>&Cccc<T>;

	// a type reference, a conditional with plain halves and an object literal do the same
	type Bbbb<T,Uuuuuuuuuuuu extends Bbbbbbbbbbbbbbbbbb=Bbbbbbbbbbbbbbbbb>=Wwwwwwwwwwwwwwwwwwwwww<T,U>;
	type Cccc<T,Uuuuuuuuuuuu extends Bbbbbbbbbbbbbbbbbb=Bbbbbbbbbbbbbbbbb>=Tttttttttt extends Wwww?A:B;
	type Dddd<T,Uuuuuuuuuuuuuuuuuuuuuuuuuuu extends Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb=Bbbbbbbbbbbbbb>={a: Wwww<T>};

	// break-after-operator still wins ahead of it: a union, a conditional with a generic check
	type Eeee<T,Uuuuuuuuuuuu extends Bbbbbbbbbbbbbbbbbb=Bbbbbbbbbbbbbbbbb> =Wwwwwwwwwwwwwwwwwwwwww<T,U>|Cccc<T>;
	type Ffff<T,Uuuuuuuuuuuu extends Bbbbbbbbbbbbbbbbbb=Bbbbbbbbbbbbbbbbb> =Tttttttttt extends Wwww<U>?A:B;

	// one parameter is never complex, however constrained: the value breaks after `=` instead
	type Gggg<Uuuuuuuuuuuuuuuuuuuuuuuuuuuuu extends Bbbbbbbbbbbbbbbbbb=Bbbbbbbbbbbbbbbbb> =Wwwwwwww<U>&Cccc<U>;
</script>
