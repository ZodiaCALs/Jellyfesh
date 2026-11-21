using Jellyfesh.Core;
using Jellyfesh.Formatting;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>Now Playing's Comments pane (docs/ui.md, "Comments").</summary>
public class CommentsTests
{
    private static readonly SourceTrack Song = new("org.x", "t1", "Lantern", "Artist", null, 180_000, null, null);

    private static PluginInfo Source(bool comments) =>
        new("org.x", "Example", "1.0.0", "Someone", PluginBadge.Official, false, ["search"], [], true, null, false, "{}", "{}", null, Comments: comments);

    private static AppHarness Start(bool comments = true)
    {
        var h = new AppHarness();
        h.Core.State = CoreSnapshot.Initial with
        {
            Sources = new SourcesState(1, [Source(comments)], [], false),
            NowPlaying = new NowPlayingInfo(3, new TrackRef.Source(Song), null, "Lantern", "Artist", null, 180_000, null, null),
        };
        return h.Started();
    }

    private static CommentEntry Entry(string id, long? timeMs = 1_700_000_000_000) => new(id, "Listener " + id, null, "Words " + id, 7, timeMs, null);

    [Fact]
    public void Comments_are_offered_only_for_sources_that_have_them()
    {
        using (var h = Start(comments: false))
        {
            Assert.False(h.App.Comments.IsAvailable);
        }

        using var with = Start();
        Assert.True(with.App.Comments.IsAvailable);
        with.Emit(new CoreEvent.NowPlaying(new NowPlayingInfo(4, new TrackRef.Library(1), "/a.flac", "Other", null, null, null, null, null)));
        Assert.False(with.App.Comments.IsAvailable);
    }

    [Fact]
    public void Opening_the_pane_asks_once_and_shows_popular_then_latest_then_more()
    {
        using var h = Start();
        var comments = h.App.Comments;
        comments.Open();
        var item = new ItemRef.Source("org.x", ItemKind.Track, "t1");
        Assert.Equal(new CoreCommand.Comments(item, null), h.Core.Sent[^1]);
        comments.Open(); // already asking
        Assert.Single(h.Core.Sent, c => c is CoreCommand.Comments);
        Assert.False(comments.IsEmpty);

        var request = (ulong)h.Core.Sent.Count;
        h.Emit(new CoreEvent.Comments(new CommentsPage(request, item, [Entry("h1")], [Entry("c1"), Entry("c2", null)], 3, "2", "Example Service", null)));
        Assert.Equal(["h1"], comments.Popular.Select(c => c.Entry.Id));
        Assert.Equal(["c1", "c2"], comments.Latest.Select(c => c.Entry.Id));
        Assert.True(comments.HasPopular);
        Assert.True(comments.HasMore);
        Assert.Equal("Comments_From", comments.Attribution);
        Assert.Equal("Comments_Likes", comments.Latest[0].Likes);
        Assert.Empty(comments.Latest[1].When);

        comments.LoadMoreCommand.Execute(null);
        Assert.Equal(new CoreCommand.Comments(item, "2"), h.Core.Sent[^1]);
        h.Emit(new CoreEvent.Comments(new CommentsPage((ulong)h.Core.Sent.Count, item, [], [Entry("c3")], 3, null, null, null)));
        Assert.Equal(["c1", "c2", "c3"], comments.Latest.Select(c => c.Entry.Id));
        Assert.False(comments.HasMore);
    }

    [Fact]
    public void No_comments_says_so_and_a_failure_shows_why()
    {
        using var h = Start();
        var comments = h.App.Comments;
        comments.Open();
        var item = new ItemRef.Source("org.x", ItemKind.Track, "t1");
        h.Emit(new CoreEvent.Comments(new CommentsPage((ulong)h.Core.Sent.Count, item, [], [], 0, null, null, null)));
        Assert.True(comments.IsEmpty);

        h.Emit(new CoreEvent.NowPlaying(new NowPlayingInfo(5, new TrackRef.Source(Song with { Id = "t2" }), null, "Two", null, null, null, null, null)));
        comments.Open();
        h.Emit(new CoreEvent.CommandFailed((ulong)h.Core.Sent.Count, "comments are closed"));
        Assert.Equal("comments are closed", comments.Error);
        Assert.False(comments.IsEmpty);
        Assert.Empty(h.App.Notifications.Items);
    }

    [Fact]
    public void Ages_read_as_people_say_them()
    {
        var l = KeyLocalizer.Instance;
        Assert.Equal("Comments_JustNow", CommentsViewModel.Age(30_000, l));
        Assert.Equal("Comments_Minutes_Other:5", CommentsViewModel.Age(5 * 60_000, l));
        Assert.Equal("Comments_Hours_One:1", CommentsViewModel.Age(90 * 60_000, l));
        Assert.Equal("Comments_Days_Other:3", CommentsViewModel.Age(3L * 86_400_000, l));
        Assert.Equal("Comments_Years_Other:2", CommentsViewModel.Age(800L * 86_400_000, l));
    }
}
