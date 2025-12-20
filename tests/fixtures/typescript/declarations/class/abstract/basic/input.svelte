<script lang="ts">
	// Abstract class with abstract method
	abstract class Base {
		abstract fn(): void;
	}

	// Abstract class with abstract property
	abstract class WithProp {
		abstract readonly value: number;
	}

	// Abstract class with abstract accessors
	abstract class WithAccessors {
		abstract get a(): string;
		abstract set a(v: string);
	}

	// Abstract class with both abstract and concrete members
	abstract class WithConcrete {
		abstract a(): void;
		b(): string {
			return 'value';
		}
	}

	// Concrete class implementing abstract class
	class Derived extends Base {
		fn(): void {
			console.log('value');
		}
	}

	// Abstract class extending abstract class
	abstract class Extended extends Base {
		abstract other(): string;
	}
</script>
