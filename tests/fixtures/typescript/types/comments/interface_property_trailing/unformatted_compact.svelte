<script lang="ts">
	interface A{a:string;// trailing comment
		b:number;}

	interface B{a:string;// comment a
		b:number;// comment b
		c:boolean;// comment c
	}

	interface C{a:string;// multiple
		// words
		b:number;}

	interface D{readonly prop1?:string;// optional readonly
		prop2?:number;// just optional
		readonly prop3:boolean;// just readonly
	}

	interface E{method():void;// method comment
		():void;// call signature comment
		[key:string]:unknown;// index signature comment
		new():object;// construct signature comment
	}

	interface F{a:string;/* first *//* second */}
</script>
