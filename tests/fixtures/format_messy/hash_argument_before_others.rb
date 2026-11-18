class BadgesController
  def mass_award
    render json: { unmatched_entries: results[:unmatched_entries].first(100), matched_users_count: results[:matched_users_count], unmatched_entries_count: results[:unmatched_entries_count] }, status: :ok
  end

  def short
    render json: { ok: true }, status: :ok
    deny_read path:, type: :subpath
  end

  # The hash is the last argument, so it stays next to its key.
  def last
    render status: :unprocessable_entity, json: { user_count: user_count, errors: [I18n.t("groups.errors.update_existing_users_required", count: user_count)] }
  end

  # A command in `( )` breaks between its arguments as it does outside.
  def request_membership
    GroupRequest.create!(group: group, user: current_user, reason: params[:reason])
  rescue ActiveRecord::RecordNotUnique
    return (render json: failed_json.merge(error: I18n.t("groups.errors.already_requested_membership")), status: :conflict)
  end

  def callback_phase
    return (fail! :missing_parameters, RuntimeError.new("Missing parameters. Requires `iss`, `login_hint` and `target_link_uri`")) if missing?
    return (redirect_to "/tags/#{permalink.target_url}/#{params[:tag_name]}", status: :moved_permanently)
  end
end
