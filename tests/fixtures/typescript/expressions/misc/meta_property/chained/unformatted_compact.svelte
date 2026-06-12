<script lang="ts">
// Deep property access on import.meta
const deep=import.meta.env.VITE_API_URL;
const deeper=import.meta.env.VITE_API_URL.toString();

// Optional chaining with import.meta
const optional=import.meta.env?.DEV;
const optionalDeep=import.meta.env?.VITE_API_URL?.length;

// import.meta in expressions
const conditional=import.meta.env.DEV?'dev':'prod';
const nullish=import.meta.env.MODE??'development';

// import.meta with method calls
const str=import.meta.url.toString();
const split=import.meta.url.split('/');
const slice=import.meta.url.slice(0,10);

// import.meta in template literal
const template=`URL: ${import.meta.url}`;

// import.meta with destructuring
const {url,env}=import.meta;
const {DEV,MODE}=import.meta.env;

// import.meta as argument
console.log(import.meta.url);
fn(import.meta.env.DEV);

// new.target with property access (rare but valid)
function Ctor(){const name=new.target?.name;const proto=new.target?.prototype;}
</script>
