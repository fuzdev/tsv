<script>
// Async function declaration
async function fn1(){return await fetch('/api');}

// Async arrow function
const fn2=async()=>{await fn3();};

// Async arrow with params
const fn4=async(id)=>{const a=await fn5(id);return a;};

// Async method shorthand (in object)
const obj={async method(){return await this.load();},};

// Await expression
const result=await promise;

// Await with member expression
const data=await obj.method();

// Await with call expression
const value=await fn6();

// Nested await
const nested=await(await fn7()).value;
</script>
