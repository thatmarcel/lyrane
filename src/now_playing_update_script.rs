use std::env::temp_dir;
use std::fs::File;
use std::io::Write;
use anyhow::anyhow;

pub const APPLE_SCRIPT_FILE_CONTENTS: &str = "
use framework \"Foundation\"

global MRNowPlayingRequest

global NSJSONSerialization
global NSString
global NSNumber
global NSNumberFormatter
global NSDecimalNumber
global NSDate

global numberFormatter

on initialize()
	set MediaRemote to current application's NSBundle's bundleWithPath: \"/System/Library/PrivateFrameworks/MediaRemote.framework\"
	MediaRemote's load()

	set MRNowPlayingRequest to current application's MRNowPlayingRequest

	set NSJSONSerialization to current application's NSJSONSerialization
	set NSString to current application's NSString
	set NSNumber to current application's NSNumber
	set NSNumberFormatter to current application's NSNumberFormatter
	set NSDecimalNumber to current application's NSDecimalNumber
	set NSDate to current application's NSDate

	set numberFormatter to NSNumberFormatter's alloc()'s init()
	numberFormatter's setMaximumFractionDigits: 3
	numberFormatter's setLocalizesFormat: false
	numberFormatter's setNumberStyle: 1 # 1 for NSNumberFormatterDecimalStyle
	numberFormatter's setFormat: \"#0.###\"
end initialize

on roundDoubleToDecimalNumber(inputDouble)
    set wrappedNumber to NSNumber's numberWithDouble: inputDouble
    set formattedNumberString to numberFormatter's stringFromNumber: wrappedNumber
    set decimalNumber to NSDecimalNumber's decimalNumberWithString: formattedNumberString

    return decimalNumber
end roundDoubleToDecimalNumber

on serializeIntoJSONString(inputObject)
	set jsonData to NSJSONSerialization's dataWithJSONObject: inputObject options: 2 |error|: (missing value) # 2 for NSJSONWritingSortedKeys
	set jsonString to (NSString's alloc()'s initWithData: jsonData encoding: 4) as string # 4 for NSUTF8StringEncoding

	return jsonString
end serializeIntoJSONString

on createNowPlayingJSONString()
    set nowPlayingContentItem to MRNowPlayingRequest's localNowPlayingItem()

  	if nowPlayingContentItem is missing value
        return \"{\\\"hasNowPlaying\\\":false}\"
    end if

    set nowPlayingContentItemMetadata to nowPlayingContentItem's metadata()

    if nowPlayingContentItemMetadata is missing value
   	    return \"{\\\"hasNowPlaying\\\":false}\"
    end if

    set npcimPlaybackRate to roundDoubleToDecimalNumber(nowPlayingContentItemMetadata's playbackRate())

    set playbackPosition to nowPlayingContentItemMetadata's elapsedTime() as real

    set elapsedTimeUpdateTimestamp to NSDate's alloc()'s initWithTimeIntervalSinceReferenceDate: (nowPlayingContentItemMetadata's elapsedTimeTimestamp())

    if elapsedTimeUpdateTimestamp is not missing value
        set currentTimestamp to NSDate's alloc()'s init()

        set timeIntervalSinceElapsedTimeUpdate to (currentTimestamp's timeIntervalSinceDate: elapsedTimeUpdateTimestamp) as real

        if timeIntervalSinceElapsedTimeUpdate is not missing value
            set playbackPosition to playbackPosition + (timeIntervalSinceElapsedTimeUpdate * (npcimPlaybackRate as real))
        end if
    end if

    set npcimTitle to nowPlayingContentItemMetadata's title()
    set npcimTrackArtistName to nowPlayingContentItemMetadata's trackArtistName()
    set npcimCalculatedPlaybackPosition to roundDoubleToDecimalNumber(playbackPosition)
    set npcimDuration to roundDoubleToDecimalNumber(nowPlayingContentItemMetadata's duration())

    set nowPlayingResultObject to { hasNowPlaying: true, title: npcimTitle, artistName: npcimTrackArtistName, currentPosition: npcimCalculatedPlaybackPosition, duration: npcimDuration, playbackRate: npcimPlaybackRate }

    set nowPlayingResultJSONString to serializeIntoJSONString(nowPlayingResultObject)

    return nowPlayingResultJSONString
end createNowPlayingJSONString

on run
	initialize()

	repeat
		try
			log createNowPlayingJSONString()
		end try

        delay 1
	end repeat
end run
";

pub fn create_now_playing_update_script_file() -> anyhow::Result<String> {
    let script_file_path = temp_dir().join("lyrane-now-playing.scpt");

    let mut script_file = File::create(&script_file_path)?;

    script_file.write_all(APPLE_SCRIPT_FILE_CONTENTS.as_bytes())?;

    Ok(script_file_path.to_str().ok_or(anyhow!("Failed to convert script file path to string"))?.to_string())
}