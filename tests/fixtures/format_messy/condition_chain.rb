class CategoryUser
  def self.batch_set(user, level_num, category_ids)
    if category_ids.present? &&
         CategoryUser
           .where(user_id: user.id, category_id: category_ids)
           .where.not(notification_level: level_num)
           .update_all(notification_level: level_num) > 0
      changed = true
    end

    if CategoryUser.where(user_id: user.id, notification_level: level_num).where.not(category_id: category_ids).delete_all > 0
      changed = true
    end

    if !SiteSetting.automatically_download_gravatars || !UserAvatar.joins(user: :user_emails).where(user_emails: { primary: true }, last_gravatar_download_attempt: nil).exists?
      rebake
    end

    # A chain that fits on the line with its operator breaks the spine the
    # usual way.
    crawled = TopicLink.where(id: topic_link.id).update_all(title: filename, crawled_at: now).count == expected_total_count_value
  end
end
