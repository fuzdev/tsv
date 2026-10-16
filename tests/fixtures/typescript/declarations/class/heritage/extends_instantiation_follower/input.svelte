<script lang="ts">
	// a superclass instantiation followed by a template is the tagged template's type arguments
	class A extends (f<T>`t`) {}
	class B extends (a.b<T>`t`) {}
	class C extends (f()<T>`t`) {}
	class D extends (f<T>`a${b}c`) {}
	// past a line break too
	class D2 extends (f<T>`t`) {}

	// a list after the tag is the class's super type arguments
	class E extends (f<T>`t`)<U> {}

	// followed by a `[` past a line break, the instantiation is the member's object
	class F extends (f<T>)[0] {}
	class G extends (f<T>)[0]<U> {}

	// the same in a class expression
	const H = class extends (f<T>`t`) {};
</script>
