user.name
user.name = "Ada"
user.save(true)
list[0]
list[0] = "first"
list[0, 1] = "a"
user&.name
user.valid?
desc = +""
x = -y
z = ~w
q = !r

short.chain.of.calls

receive(:readline).with("y").and_return("y")

expect(Thor::LineEditor).to receive(:readline)
  .with("Should I overwrite it? ", { add_to_history: false })
  .and_return("y")

expect(Thor::LineEditor).to receive(:readline)
  .with(
    "What's your favorite Neopolitan flavor? [strawberry, chocolate, vanilla] ",
    { limited_to: flavors, case_insensitive: true }
  )
  .and_return("CHOCOLATE")

# A chain hanging off a receiver that laid out its own body continues the
# line that body ends on, and is measured from the column it really starts
# at rather than from the whole body's width.
def to_gid_param
  attributes.map do |k, o|
    case o
    when String
      o
    else
      o.to_s
    end
  end.sort.join(":")
end

# An argument list inside a chain link that has to expand indents from the
# link, not from the statement.
module ActionDispatch
  class Cookies
    def prepare_upgrade_legacy_hmac_aes_cbc_cookies
      future_cipher = encrypted_cookie_cipher
      secret = request
        .key_generator
        .generate_key(
          request.authenticated_encrypted_cookie_salt,
          ActiveSupport::MessageEncryptor.key_len(future_cipher)
        )
    end
  end
end
