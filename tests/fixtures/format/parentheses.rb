colwidth = (array.map { |el| el.to_s.size }.max || 0) + 2
result = ((index + 1) % (Terminal.terminal_width / colwidth)).zero? && !index.zero?
value = (1 + 2) * 3

# A parenthesized body that overflows wraps inside the parens, at one indent
# from the statement -- the parens belong to the expression, so the
# continuation does not align under them.
PROTECTED_IVARS = (AbstractController::Rendering::DEFAULT_PROTECTED_INSTANCE_VARIABLES +
  [:@_action_has_layout, :@_response_body]).freeze

def self.===(other)
  super ||
    (defined?(Rack::Utils::ParameterTypeError) && Rack::Utils::ParameterTypeError === other ||
      defined?(Rack::Utils::InvalidParameterError) && Rack::Utils::InvalidParameterError === other)
end

# A body holding a construct with no one-line spelling expands rather than
# joining its parts with `;`.
def inside_repository?(file_path, api_path)
  file_path.ascend.none?(repository_path) && !(api_path && begin
    file_path.expand_path.ascend.any?(api_path)
  rescue Errno::ENOENT
    false
  end)
end
