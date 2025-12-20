<script lang="ts">
	class Child extends Parent {
		constructor() {
			super();
		}
		greet() {
			return super.greet();
		}
		getName() {
			return super.name;
		}
		getItem(index) {
			return super[index];
		}
	}
</script>
