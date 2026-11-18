describe "comments" do
  it "keeps a directive above the value it covers" do
    expect(Notification).to have_received(:create!).with(
      processed: 2,
      logs:
        # rubocop:disable Layout/LineLength
        "Couldn't find user or group: 'non_existent' or the groups provided contained no users. Note that public events can't bulk invite groups.",
      # rubocop:enable Layout/LineLength
    )
  end

  it "keeps a modifier's comment on its statement" do
    all_invoices.each do |invoice|
      line_item = invoice[:lines][:data][0] if invoice[:lines] && invoice[:lines][:data] # Discourse only makes single-line item charges
      short = 1 if ok # fits
    end
  end

  it "keeps a one-line block's comment at the end" do
    expect do post.rebake! end.not_to change { Upload.count } # We already have the upload
  end
end
