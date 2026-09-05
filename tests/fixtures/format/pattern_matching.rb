assert_pattern { @params => { person: { age: "32" } } }
assert_pattern { response.parsed_body => { foo: /fighter/ } }
assert_pattern { h1 => { name: "h1", content: "Hello, world", attributes: [{ name: "id", value: "hello-world" }] } }
x = 1
h = { x: }
n in Prism::CaseNode[predicate: Prism::CallNode[receiver: Prism::LocalVariableReadNode[name: :target_version]]]
x in { person:, age: }

def visit_module_node(node)
  case node.constant_path
  in Prism::ConstantReadNode[name:]
    @namespace_stack << name
  in Prism::ConstantPathNode
    @namespace_stack << node.constant_path.full_name
  end
end
