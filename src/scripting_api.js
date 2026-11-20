// Only these objects bridge the engine to the application. No module loader or system APIs.
(() => {
  'use strict';
  const input = JSON.parse(globalThis.__zephInput);
  delete globalThis.__zephInput;
  const uuid = globalThis.__zephUuid;
  delete globalThis.__zephUuid;
  const stringify = JSON.stringify.bind(JSON);
  const parse = JSON.parse.bind(JSON);
  const clone = value => parse(stringify(value));
  const short = (value, limit) => {
    const output = String(value).slice(0, limit);
    return output.toWellFormed();
  };
  const result = { variables: Object.create(null), environment: Object.create(null), request: null, logs: [], tests: [], error: null };
  const vars = Object.assign(Object.create(null), input.variables);
  const env = Object.assign(Object.create(null), input.environment);
  const request = input.request;
  const pre = input.stage === 'Pre';
  const editable = () => { if (!pre) throw new Error('后置脚本不能修改请求'); };
  let writes = 0;
  const text = value => typeof value === 'string' ? value : stringify(value);
  const store = (target, changes, key, value) => {
    key = String(key).trim();
    value = text(value);
    if (!key || /[{}]/.test(key) || key.length > 256) throw new Error('变量名无效');
    if (typeof value !== 'string' || value.length > 32768) throw new Error('变量值无效或超过 32K 字符');
    if (++writes > 128) throw new Error('每个脚本最多写入 128 次变量');
    target[key] = value;
    changes[key] = value;
    vars[key] = value;
    result.variables[key] = value;
  };
  const variables = Object.freeze({ get:key => vars[String(key)], has:key => Object.hasOwn(vars,String(key)), set:(key,value) => store(vars,result.variables,key,value) });
  const environment = Object.freeze({ get:key => env[String(key)], has:key => Object.hasOwn(env,String(key)), set:(key,value) => store(env,result.environment,key,value) });
  const collection = (field, insensitive) => {
    const equal = (a,b) => insensitive ? a.toLowerCase() === b.toLowerCase() : a === b;
    return Object.freeze({
      get: name => request[field].find(p => p.enabled && equal(p.name,String(name)))?.value,
      all: () => clone(request[field]),
      set(name,value) { editable(); name=String(name); value=String(value); const p=request[field].find(p => equal(p.name,name)); if(p){p.value=value;p.enabled=true;}else{this.add(name,value);} },
      add(name,value) { editable(); if(request[field].length>=256)throw new Error('参数超过 256 项'); request[field].push({name:String(name),value:String(value),enabled:true,description:'',file:false}); },
      remove(name) { editable(); request[field]=request[field].filter(p => !equal(p.name,String(name))); }
    });
  };
  const body = Object.freeze({
    get raw(){return request.body;},
    set raw(value){editable();request.body=String(value);if(request.body_mode==='None')request.body_mode='Text';},
    get mode(){return request.body_mode;},
    json(){return parse(request.body);},
    setJson(value){editable();request.body_mode='Json';request.body=stringify(value);},
    setText(value){editable();request.body_mode='Text';request.body=String(value);}
  });
  const requestApi=Object.freeze({
    get url(){return request.url;}, set url(value){editable();request.url=String(value);},
    get method(){return request.method;},set method(value){editable();request.method=String(value).trim().toUpperCase();},
    headers:collection('headers',true),query:collection('query',false),path:collection('path',false),body
  });
  const response = input.response;
  const pairs = values => Object.freeze({get:name=>values.find(([key])=>key.toLowerCase()===String(name).toLowerCase())?.[1],all:()=>clone(values)});
  const responseApi = response ? Object.freeze({
    code:response.code,status:response.status,elapsedMs:response.elapsed_ms,truncated:response.truncated,
    headers:pairs(response.headers),trailers:pairs(response.trailers),
    text:()=>response.body,
    json:()=>{if(response.truncated)throw new Error('响应已截断，无法可靠处理 JSON');return parse(response.body);},
    messages:()=>clone(response.messages)
  }):null;
  const assert = (condition,message='断言失败') => {if(!condition)throw new Error(String(message));};
  const test = (name,fn) => {
    if(result.tests.length>=64)throw new Error('每个脚本最多 64 项测试');
    const entry={name:short(name,256),passed:false,error:null};
    result.tests.push(entry);
    try {const returned=fn();if(returned && typeof returned.then==='function')throw new Error('测试仅支持同步函数');entry.passed=true;}
    catch(e){entry.error=short(e,2048);}
  };
  const log = level => (...args) => {
    if(result.logs.length>=50)return;
    const message=args.map(value=>{try{return typeof value==='string'?value:stringify(value);}catch{return '[无法序列化]';}}).join(' ');
    result.logs.push(short(`${level}  ${message}`,2048));
  };
  const api=Object.freeze({variables,environment,request:requestApi,response:responseApi,test,assert,uuid});
  Object.defineProperty(globalThis,'zeph',{value:api,writable:false,configurable:false});
  Object.defineProperty(globalThis,'console',{value:Object.freeze({log:log('info'),info:log('info'),warn:log('warn'),error:log('error')}),writable:false,configurable:false});
  Object.defineProperty(globalThis,'__zephFinish',{value:()=>{
    result.request=pre?request:null;
    return stringify(result);
  },writable:false,configurable:false});
})();
